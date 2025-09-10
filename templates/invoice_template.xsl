<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet version="1.0"
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:fo="http://www.w3.org/1999/XSL/Format">

    <!-- Page Layout Definition -->
    <fo:layout-master-set>
        <fo:simple-page-master master-name="invoice-page"
                               page-width="612pt"
                               page-height="792pt"
                               margin-top="40pt"
                               margin-bottom="40pt"
                               margin-left="40pt"
                               margin-right="40pt"
                               footer-text="Thank you for your business!"
                               footer-style="footer"/>
    </fo:layout-master-set>

    <!-- Style Definitions -->
    <xsl:attribute-set name="h1">
        <xsl:attribute name="font-size">28pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="color">#003366</xsl:attribute>
        <xsl:attribute name="margin-bottom">25pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="h2">
        <xsl:attribute name="font-size">14pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="color">#666666</xsl:attribute>
        <xsl:attribute name="margin-bottom">5pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="addressBlock">
        <xsl:attribute name="margin-bottom">30pt</xsl:attribute>
        <xsl:attribute name="padding">10pt</xsl:attribute>
        <xsl:attribute name="background-color">#F5F5F5</xsl:attribute>
        <xsl:attribute name="border">1pt solid #DCDCDC</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="body">
        <xsl:attribute name="font-size">11pt</xsl:attribute>
        <xsl:attribute name="line-height">14pt</xsl:attribute>
        <xsl:attribute name="margin-bottom">2pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="invoiceMeta">
        <xsl:attribute name="font-size">11pt</xsl:attribute>
        <xsl:attribute name="text-align">right</xsl:attribute>
        <xsl:attribute name="margin-bottom">2pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="hr">
        <xsl:attribute name="height">2pt</xsl:attribute>
        <xsl:attribute name="background-color">#003366</xsl:attribute>
        <xsl:attribute name="margin">5pt 0pt 30pt 0pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="invoiceTable">
        <xsl:attribute name="background-color">#F0F2F5</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="th">
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="text-align">left</xsl:attribute>
        <xsl:attribute name="color">#FFFFFF</xsl:attribute>
        <xsl:attribute name="background-color">#465569</xsl:attribute>
        <xsl:attribute name="padding">8pt 5pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="td">
        <xsl:attribute name="padding">8pt 5pt</xsl:attribute>
        <xsl:attribute name="border">0.5pt solid #FFFFFF</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="td-right">
        <xsl:attribute name="text-align">right</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="footer">
        <xsl:attribute name="font-size">10pt</xsl:attribute>
        <xsl:attribute name="text-align">center</xsl:attribute>
        <xsl:attribute name="color">#888888</xsl:attribute>
    </xsl:attribute-set>

    <!-- Root Template -->
    <xsl:template match="/">
        <document>
            <!-- The page-sequence creates a new page FOR EACH customer -->
            <page-sequence select="/customers">
                <text style="h1">INVOICE</text>
                <rectangle style="hr"/>

                <container style="addressBlock">
                    <text style="h2">BILL TO</text>
                    <text style="body"><xsl:value-of select="name"/></text>
                    <text style="body"><xsl:value-of select="address"/></text>
                </container>

                <text style="invoiceMeta">Invoice Number: <xsl:value-of select="invoiceNumber"/></text>
                <text style="invoiceMeta">Date: 2023-10-27</text>

                <table>
                    <columns>
                        <column width="60%" header-style="th" style="td"/>
                        <column width="20%" header-style="th" style="td-right"/>
                        <column width="20%" header-style="th" style="td-right"/>
                    </columns>
                    <header>
                        <row>
                            <cell>Product / Service</cell>
                            <cell>Quantity</cell>
                            <cell>Unit Price</cell>
                        </row>
                    </header>
                    <tbody>
                        <xsl:for-each select="items">
                            <row>
                                <cell><xsl:value-of select="product"/></cell>
                                <cell><xsl:value-of select="quantity"/></cell>
                                <cell><xsl:value-of select="price"/></cell>
                            </row>
                        </xsl:for-each>
                    </tbody>
                </table>
            </page-sequence>
        </document>
    </xsl:template>
</xsl:stylesheet>