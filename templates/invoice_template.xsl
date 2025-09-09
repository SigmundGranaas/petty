<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet version="1.0"
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:petty="https://docs.petty.dev/ns/1.0">

    <!--
      Page layout is defined using a custom namespaced tag. Its attributes
      are parsed to configure the page size and margins for the document.
    -->
    <petty:page-layout size="Letter" margin="40pt" />

    <!--
      Styles are defined using the standard XSLT <xsl:attribute-set> element.
      The 'name' of the set can be referenced by 'style' attributes on layout
      tags like <text> or <container>.
    -->
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
    </xsl:attribute-set>
    <xsl:attribute-set name="body">
        <xsl:attribute name="font-size">11pt</xsl:attribute>
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
        <xsl:attribute name="margin-top">5pt</xsl:attribute>
        <xsl:attribute name="margin-bottom">30pt</xsl:attribute>
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
    </xsl:attribute-set>
    <xsl:attribute-set name="td-right">
        <xsl:attribute name="padding">8pt 5pt</xsl:attribute>
        <xsl:attribute name="text-align">right</xsl:attribute>
    </xsl:attribute-set>


    <!-- The main template that processes the incoming JSON data -->
    <xsl:template match="/">
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

            <table style="invoiceTable">
                <columns>
                    <column width="50%" />
                    <column width="25%" />
                    <column width="25%" />
                </columns>
                <header>
                    <row>
                        <cell style="th">Product / Service</cell>
                        <cell style="th">Quantity</cell>
                        <cell style="th">Unit Price</cell>
                    </row>
                </header>
                <tbody>
                    <xsl:for-each select="items">
                        <row>
                            <cell style="td"><xsl:value-of select="product"/></cell>
                            <cell style="td-right"><xsl:value-of select="quantity"/></cell>
                            <cell style="td-right">{{formatCurrency price}}</cell>
                        </row>
                    </xsl:for-each>
                </tbody>
            </table>
        </page-sequence>
    </xsl:template>

</xsl:stylesheet>