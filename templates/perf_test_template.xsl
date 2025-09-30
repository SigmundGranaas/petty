<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet version="1.0"
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:fo="http://www.w3.org/1999/XSL/Format">

    <!-- 1. PAGE LAYOUT -->
    <fo:simple-page-master
            page-width="8.5in"
            page-height="11in"
            margin="50pt"
            footer-text="Report ID: {{id}} | Page {{page_num}}"
            footer-style="footer"
    />

    <!-- 2. STYLE DEFINITIONS (Attribute Sets) -->
    <xsl:attribute-set name="page-title">
        <xsl:attribute name="font-size">28pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="color">#0D47A1</xsl:attribute>
        <xsl:attribute name="margin-bottom">25pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="account-info-text">
        <xsl:attribute name="font-size">12pt</xsl:attribute>
        <xsl:attribute name="color">#555555</xsl:attribute>
        <xsl:attribute name="margin-bottom">35pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="section-title">
        <xsl:attribute name="font-size">18pt</xsl:attribute>
        <xsl:attribute name="color">#0D47A1</xsl:attribute>
        <xsl:attribute name="margin-bottom">15pt</xsl:attribute>
        <xsl:attribute name="padding-bottom">5pt</xsl:attribute>
        <xsl:attribute name="border-bottom">1pt solid #B0BEC5</xsl:attribute>
    </xsl:attribute-set>

    <!-- Table Styles -->
    <xsl:attribute-set name="transaction-table">
        <xsl:attribute name="margin-bottom">25pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="th">
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="text-align">left</xsl:attribute>
        <xsl:attribute name="padding">8pt 6pt</xsl:attribute>
        <xsl:attribute name="border-bottom">2pt solid #0D47A1</xsl:attribute>
        <xsl:attribute name="color">#37474F</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="th-right" use-attribute-sets="th">
        <xsl:attribute name="text-align">right</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="td-odd">
        <xsl:attribute name="padding">7pt 6pt</xsl:attribute>
        <xsl:attribute name="font-size">10pt</xsl:attribute>
        <xsl:attribute name="border-bottom">1pt solid #ECEFF1</xsl:attribute>
        <xsl:attribute name="color">#263238</xsl:attribute>
    </xsl:attribute-set>
    <xsl:attribute-set name="td-odd-right" use-attribute-sets="td-odd">
        <xsl:attribute name="text-align">right</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="td-even" use-attribute-sets="td-odd">
        <xsl:attribute name="background-color">#F8F9FA</xsl:attribute>
    </xsl:attribute-set>
    <xsl:attribute-set name="td-even-right" use-attribute-sets="td-even">
        <xsl:attribute name="text-align">right</xsl:attribute>
    </xsl:attribute-set>

    <!-- Summary Table Styles -->
    <xsl:attribute-set name="summary-table">
        <xsl:attribute name="margin-top">20pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="summary-label">
        <xsl:attribute name="text-align">right</xsl:attribute>
        <xsl:attribute name="padding">5pt 10pt</xsl:attribute>
        <xsl:attribute name="font-size">11pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="summary-value" use-attribute-sets="summary-label">
        <xsl:attribute name="padding">5pt 6pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="summary-total-label" use-attribute-sets="summary-label">
        <xsl:attribute name="font-size">14pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="border-top">1.5pt solid #263238</xsl:attribute>
        <xsl:attribute name="padding-top">10pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="summary-total-value" use-attribute-sets="summary-total-label">
        <xsl:attribute name="padding">10pt 6pt 5pt 6pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="footer">
        <xsl:attribute name="font-size">9pt</xsl:attribute>
        <xsl:attribute name="color">#78909C</xsl:attribute>
        <xsl:attribute name="text-align">center</xsl:attribute>
    </xsl:attribute-set>


    <!-- 3. DOCUMENT STRUCTURE TEMPLATE -->
    <xsl:template match="/">
        <page-sequence>
            <text use-attribute-sets="page-title">Transaction Summary</text>

            <!-- *** KEY FIX 1: Corrected data paths to match the JSON structure from main.rs *** -->
            <text use-attribute-sets="account-info-text">Account: <xsl:value-of select="user/account"/></text>

            <text use-attribute-sets="section-title">Details</text>

            <table use-attribute-sets="transaction-table">
                <columns>
                    <column width="auto"/>
                    <column width="15%"/>
                    <column width="20%"/>
                    <column width="20%"/>
                </columns>
                <header>
                    <row>
                        <cell use-attribute-sets="th"><text>Item</text></cell>
                        <cell use-attribute-sets="th-right"><text>Qty</text></cell>
                        <cell use-attribute-sets="th-right"><text>Unit Price</text></cell>
                        <cell use-attribute-sets="th-right"><text>Total</text></cell>
                    </row>
                </header>
                <tbody>
                    <!-- The path is now 'items' directly, not 'records/items' -->
                    <xsl:for-each select="items">
                        <row>
                            <xsl:choose>
                                <xsl:when test="position() mod 2 = 1">
                                    <cell use-attribute-sets="td-odd"><text><xsl:value-of select="description"/></text></cell>
                                    <cell use-attribute-sets="td-odd-right"><text><xsl:value-of select="quantity"/></text></cell>
                                    <cell use-attribute-sets="td-odd-right"><text>{{formatCurrency price}}</text></cell>
                                    <cell use-attribute-sets="td-odd-right"><text>{{formatCurrency line_total}}</text></cell>
                                </xsl:when>
                                <xsl:otherwise>
                                    <cell use-attribute-sets="td-even"><text><xsl:value-of select="description"/></text></cell>
                                    <cell use-attribute-sets="td-even-right"><text><xsl:value-of select="quantity"/></text></cell>
                                    <cell use-attribute-sets="td-even-right"><text>{{formatCurrency price}}</text></cell>
                                    <cell use-attribute-sets="td-even-right"><text>{{formatCurrency line_total}}</text></cell>
                                </xsl:otherwise>
                            </xsl:choose>
                        </row>
                    </xsl:for-each>
                </tbody>
            </table>

            <!--
              *** KEY FIX 2: Replaced fragile flexbox with a robust three-column table. ***
              The first 'auto' column acts as a spacer, pushing the other two to the right.
              This guarantees perfect alignment within the page boundaries.
            -->
            <table use-attribute-sets="summary-table">
                <columns>
                    <column width="auto"/>
                    <column width="25%"/>
                    <column width="20%"/>
                </columns>
                <tbody>
                    <row>
                        <cell><text/></cell> <!-- Empty spacer cell -->
                        <cell use-attribute-sets="summary-label"><text>Subtotal:</text></cell>
                        <cell use-attribute-sets="summary-value"><text>{{formatCurrency summary/total}}</text></cell>
                    </row>
                    <row>
                        <cell><text/></cell> <!-- Empty spacer cell -->
                        <cell use-attribute-sets="summary-label"><text>Tax (8%):</text></cell>
                        <cell use-attribute-sets="summary-value"><text>{{formatCurrency summary/tax}}</text></cell>
                    </row>
                    <row>
                        <cell><text/></cell> <!-- Empty spacer cell -->
                        <cell use-attribute-sets="summary-total-label"><text>Grand Total:</text></cell>
                        <cell use-attribute-sets="summary-total-value"><text>{{formatCurrency summary/grand_total}}</text></cell>
                    </row>
                </tbody>
            </table>
        </page-sequence>
    </xsl:template>
</xsl:stylesheet>