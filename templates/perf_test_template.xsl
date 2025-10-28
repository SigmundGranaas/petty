<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet version="1.0"
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:fo="http://www.w3.org/1999/XSL/Format">

    <!-- 1. PAGE LAYOUT -->
    <fo:simple-page-master page-width="8.5in" page-height="11in" margin="50pt"/>

    <!-- 2. STYLE DEFINITIONS (Attribute Sets) -->
    <xsl:attribute-set name="page-title"><xsl:attribute name="font-size">28pt</xsl:attribute><xsl:attribute name="font-weight">bold</xsl:attribute><xsl:attribute name="color">#0D47A1</xsl:attribute><xsl:attribute name="margin-bottom">25pt</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="account-info-text"><xsl:attribute name="font-size">12pt</xsl:attribute><xsl:attribute name="color">#555555</xsl:attribute><xsl:attribute name="margin-bottom">35pt</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="section-title"><xsl:attribute name="font-size">18pt</xsl:attribute><xsl:attribute name="color">#0D47A1</xsl:attribute><xsl:attribute name="margin-bottom">15pt</xsl:attribute><xsl:attribute name="padding-bottom">5pt</xsl:attribute><xsl:attribute name="border-bottom">1pt solid #B0BEC5</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="transaction-table"><xsl:attribute name="margin-bottom">25pt</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="th"><xsl:attribute name="font-weight">bold</xsl:attribute><xsl:attribute name="text-align">left</xsl:attribute><xsl:attribute name="padding">8pt 6pt</xsl:attribute><xsl:attribute name="border-bottom">2pt solid #0D47A1</xsl:attribute><xsl:attribute name="color">#37474F</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="th-right" use-attribute-sets="th"><xsl:attribute name="text-align">right</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="td-odd"><xsl:attribute name="padding">7pt 6pt</xsl:attribute><xsl:attribute name="border-bottom">1pt solid #ECEFF1</xsl:attribute><xsl:attribute name="color">#263238</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="td-odd-right" use-attribute-sets="td-odd"><xsl:attribute name="text-align">right</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="td-even" use-attribute-sets="td-odd"><xsl:attribute name="background-color">#F8F9FA</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="td-even-right" use-attribute-sets="td-even"><xsl:attribute name="text-align">right</xsl:attribute></xsl:attribute-set>

    <!-- FIX 1: Replaced the flex-container style with a simple container style -->
    <xsl:attribute-set name="summary-container">
        <xsl:attribute name="margin-top">20pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="summary-label"><xsl:attribute name="text-align">right</xsl:attribute><xsl:attribute name="padding">5pt 10pt</xsl:attribute><xsl:attribute name="font-weight">bold</xsl:attribute><xsl:attribute name="color">#555</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="summary-value" use-attribute-sets="summary-label"><xsl:attribute name="font-weight">normal</xsl:attribute><xsl:attribute name="padding">5pt 6pt</xsl:attribute><xsl:attribute name="color">#263238</xsl:attribute><xsl:attribute name="text-align">right</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="summary-total-label" use-attribute-sets="summary-label"><xsl:attribute name="font-weight">bold</xsl:attribute><xsl:attribute name="color">#0D47A1</xsl:attribute><xsl:attribute name="border-top">1.5pt solid #263238</xsl:attribute><xsl:attribute name="padding-top">10pt</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="summary-total-value" use-attribute-sets="summary-total-label"><xsl:attribute name="padding">10pt 6pt 5pt 6pt</xsl:attribute><xsl:attribute name="font-weight">normal</xsl:attribute><xsl:attribute name="text-align">right</xsl:attribute></xsl:attribute-set>
    <xsl:attribute-set name="footer"><xsl:attribute name="font-size">9pt</xsl:attribute><xsl:attribute name="color">#78909C</xsl:attribute><xsl:attribute name="text-align">center</xsl:attribute></xsl:attribute-set>

    <!-- 3. DOCUMENT STRUCTURE TEMPLATE -->
    <xsl:template match="/">
        <xsl:apply-templates select="root"/>
    </xsl:template>

    <xsl:template match="root">
        <fo:block>
            <p use-attribute-sets="page-title">Transaction Summary</p>
            <p use-attribute-sets="account-info-text">Account: <xsl:value-of select="user/account"/></p>
            <p use-attribute-sets="section-title">Details</p>

            <table use-attribute-sets="transaction-table">
                <columns>
                    <column width="auto"/>
                    <column width="15%"/>
                    <column width="20%"/>
                    <column width="20%"/>
                </columns>
                <header>
                    <row>
                        <cell use-attribute-sets="th"><p>Item</p></cell>
                        <cell use-attribute-sets="th-right"><p>Qty</p></cell>
                        <cell use-attribute-sets="th-right"><p>Unit Price</p></cell>
                        <cell use-attribute-sets="th-right"><p>Total</p></cell>
                    </row>
                </header>
                <tbody>
                    <xsl:for-each select="items/item">
                        <row>
                            <xsl:choose>
                                <xsl:when test="position() mod 2 = 1">
                                    <cell use-attribute-sets="td-odd"><p><xsl:value-of select="description"/></p></cell>
                                    <cell use-attribute-sets="td-odd-right"><p><xsl:value-of select="quantity"/></p></cell>
                                    <cell use-attribute-sets="td-odd-right"><p><xsl:value-of select="price"/></p></cell>
                                    <cell use-attribute-sets="td-odd-right"><p><xsl:value-of select="line_total"/></p></cell>
                                </xsl:when>
                                <xsl:otherwise>
                                    <cell use-attribute-sets="td-even"><p><xsl:value-of select="description"/></p></cell>
                                    <cell use-attribute-sets="td-even-right"><p><xsl:value-of select="quantity"/></p></cell>
                                    <cell use-attribute-sets="td-even-right"><p><xsl:value-of select="price"/></p></cell>
                                    <cell use-attribute-sets="td-even-right"><p><xsl:value-of select="line_total"/></p></cell>
                                </xsl:otherwise>
                            </xsl:choose>
                        </row>
                    </xsl:for-each>
                </tbody>
            </table>

            <!-- FIX 2: Replaced flex-container with a simple block. -->
            <fo:block use-attribute-sets="summary-container">
                <table>
                    <!-- FIX 3: Added a flexible spacer column to push the other two columns to the right. -->
                    <columns>
                        <column width="60%"/> <!-- Flexible Spacer -->
                        <column width="auto"/> <!-- Label Column -->
                        <column width="auto"/> <!-- Value Column -->
                    </columns>
                    <tbody>
                        <row>
                            <!-- FIX 4: Added an empty cell to occupy the spacer column. -->
                            <cell/>
                            <cell use-attribute-sets="summary-label"><p>Subtotal:</p></cell>
                            <cell use-attribute-sets="summary-value"><p><xsl:value-of select="summary/total"/></p></cell>
                        </row>
                        <row>
                            <cell/>
                            <cell use-attribute-sets="summary-label"><p>Tax (8%):</p></cell>
                            <cell use-attribute-sets="summary-value"><p><xsl:value-of select="summary/tax"/></p></cell>
                        </row>
                        <row>
                            <cell/>
                            <cell use-attribute-sets="summary-total-label"><p>Grand Total:</p></cell>
                            <cell use-attribute-sets="summary-total-value"><p><xsl:value-of select="summary/grand_total"/></p></cell>
                        </row>
                    </tbody>
                </table>
            </fo:block>
        </fo:block>
    </xsl:template>
</xsl:stylesheet>